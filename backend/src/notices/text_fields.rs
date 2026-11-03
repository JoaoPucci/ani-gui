//! The checks behind a published notice's id and text fields (see
//! [`super::remote`]). Each returns the value to pass on or the reason
//! the notice is dropped.

use serde_json::Value;

use super::remote::MAX_ID_CHARS;

pub(super) fn id(raw: Option<&Value>) -> Result<String, &'static str> {
    let s = raw.and_then(Value::as_str).ok_or("a notice has no id")?;
    let ok = !s.is_empty()
        && s.chars().count() <= MAX_ID_CHARS
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        });
    if ok {
        Ok(s.to_string())
    } else {
        Err("a notice's id is not lower-case letters, digits, '.', '_' or '-' within the limit")
    }
}

/// A text field: absent is `None`; present must be a non-empty string
/// within `max` characters once trimmed, with no control characters
/// (a line break only where `multiline`) and no direction overrides.
pub(super) fn text(
    raw: Option<&Value>,
    max: usize,
    multiline: bool,
) -> Result<Option<String>, &'static str> {
    let Some(raw) = raw else { return Ok(None) };
    let s = raw.as_str().ok_or("a text field is not a string")?;
    let s = s.replace("\r\n", "\n");
    let s = s.trim();
    if s.is_empty() {
        return Err("a text field is empty");
    }
    if s.chars().count() > max {
        return Err("a text field is past its limit");
    }
    if s.chars().any(|c| !allowed_char(c, multiline)) {
        return Err("a text field carries a control or direction character");
    }
    Ok(Some(s.to_string()))
}

fn allowed_char(c: char, multiline: bool) -> bool {
    if c == '\n' {
        return multiline;
    }
    let bidi =
        matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}');
    !c.is_control() && !bidi
}
