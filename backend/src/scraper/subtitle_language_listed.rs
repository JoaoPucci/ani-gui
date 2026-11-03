//! A provider's own `lang` field read as a tag; split from [`super`]
//! so each file stays inside the CRAP gate's per-file bar.

use super::track_tag;

/// The tag a provider's own `lang` field stands for, beside the track's
/// label. A field shaped like a tag — a two- or three-letter language,
/// then a script or a region, `_` or `-` between — stands, in the
/// tag's own casing (`pt_BR` is `pt-BR`), whether or not the table
/// knows the language. Anything else — a name (`English`,
/// `Portuguese (Brazil)`) — is read as the label would be, and failing
/// that the label is; a track neither names is `und`.
#[must_use]
pub fn listed_tag(lang: &str, label: &str) -> String {
    tag_shaped(lang)
        .or_else(|| track_tag("", lang))
        .or_else(|| track_tag("", label))
        .unwrap_or_else(|| "und".to_string())
}

/// `lang` in a tag's casing, when it is shaped like a tag.
fn tag_shaped(lang: &str) -> Option<String> {
    let mut parts = lang.trim().split(['-', '_']);
    let language = parts.next()?.to_ascii_lowercase();
    let letters = |s: &str| s.bytes().all(|b| b.is_ascii_alphabetic());
    if !(2..=3).contains(&language.len()) || !letters(&language) {
        return None;
    }
    let mut tag = language;
    let mut script = false;
    let mut region = false;
    for part in parts {
        let digits = part.len() == 3 && part.bytes().all(|b| b.is_ascii_digit());
        match part.len() {
            4 if letters(part) && !script && !region => {
                script = true;
                tag.push('-');
                tag.push_str(&part[..1].to_ascii_uppercase());
                tag.push_str(&part[1..].to_ascii_lowercase());
            }
            2 if letters(part) && !region => {
                region = true;
                tag.push('-');
                tag.push_str(&part.to_ascii_uppercase());
            }
            3 if digits && !region => {
                region = true;
                tag.push('-');
                tag.push_str(part);
            }
            _ => return None,
        }
    }
    Some(tag)
}

#[cfg(test)]
#[path = "subtitle_language_listed_test.rs"]
mod tests;
