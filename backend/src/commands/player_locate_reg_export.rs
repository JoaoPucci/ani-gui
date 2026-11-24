//! Reading a `reg export` file, which reg.exe writes as UTF-16. Split
//! from [`super`] so each file stays inside the CRAP gate's per-file
//! bar.

/// The key's default value from a `reg export` file: the file's bytes,
/// UTF-16 as reg.exe writes them, so a path in any language comes back
/// as written. The first key's `@` line only; a REG_SZ value is quoted
/// and escaped, a REG_EXPAND_SZ one written as `hex(2):` bytes.
#[must_use]
pub fn parse_reg_export(bytes: &[u8]) -> Option<String> {
    let body = bytes.strip_prefix(&[0xFF, 0xFE]).unwrap_or(bytes);
    let units: Vec<u16> = body
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let text = String::from_utf16(&units).ok()?;
    // A long value is continued over lines ending in `\`.
    let joined = text.replace("\\\r\n", "").replace("\\\n", "");
    let mut keys = 0;
    for line in joined.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            keys += 1;
            continue;
        }
        if keys != 1 {
            continue;
        }
        let Some(value) = line.strip_prefix("@=") else {
            continue;
        };
        let value = if let Some(quoted) = value.strip_prefix('"') {
            unescape_reg_string(quoted.strip_suffix('"')?)
        } else {
            decode_hex2(value.strip_prefix("hex(2):")?)?
        };
        let value = value.trim().trim_matches('"').trim();
        return (!value.is_empty()).then(|| value.to_string());
    }
    None
}

/// A quoted .reg value's text: `\\` and `\"` stand for `\` and `"`.
fn unescape_reg_string(escaped: &str) -> String {
    let mut out = String::new();
    let mut chars = escaped.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A `hex(2):` value: comma-separated bytes of UTF-16LE text, up to its
/// first NUL.
fn decode_hex2(list: &str) -> Option<String> {
    let bytes: Vec<u8> = list
        .split(',')
        .map(str::trim)
        .filter(|b| !b.is_empty())
        .map(|b| u8::from_str_radix(b, 16).ok())
        .collect::<Option<_>>()?;
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&u| u != 0)
        .collect();
    String::from_utf16(&units).ok()
}
