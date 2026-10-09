//! Spelling `{$name}` references out of a playlist's text: a URI line
//! whole, and a tag's line only in the values the spec lets carry them.

use super::Variables;

/// A tag's line with references spelled in its quoted-string values and
/// its hexadecimal-sequence values — an unquoted value that reads `0x…`
/// once spelled, whether it was written so or a reference spells it
/// whole — and nowhere else.
pub(super) fn spell_attributes(line: &str, vars: &Variables) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while !rest.is_empty() {
        if let Some(quoted) = rest.strip_prefix('"') {
            let end = quoted.find('"').map_or(quoted.len(), |end| end + 1);
            out.push('"');
            out.push_str(&spell(&quoted[..end], vars));
            rest = &quoted[end..];
        } else if let Some(value) = rest.strip_prefix('=') {
            let end = value.find([',', '"', '\r', '\n']).unwrap_or(value.len());
            let spelled = spell(&value[..end], vars);
            let hex = spelled.starts_with("0x") || spelled.starts_with("0X");
            out.push('=');
            out.push_str(if hex { &spelled } else { &value[..end] });
            rest = &value[end..];
        } else {
            let end = rest.find(['"', '=']).unwrap_or(rest.len());
            out.push_str(&rest[..end]);
            rest = &rest[end..];
        }
    }
    out
}

/// `text` with each `{$name}` reference to a variable in `vars`
/// replaced by its value, in one pass: an inserted value is not read
/// for references.
pub(super) fn spell(text: &str, vars: &Variables) -> String {
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
