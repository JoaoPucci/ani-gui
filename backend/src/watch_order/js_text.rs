//! Text read as the renderer's JavaScript read it, where the watch
//! order's rules came from: `\s` whitespace, and `Number(text)`.

/// Whether JavaScript's `\s` (and `String.prototype.trim`) counts `c`
/// as whitespace.
#[must_use]
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// A decimal literal as `Number()` takes it: digits with at most one
/// point, at least one digit, and an exponent after them.
fn is_decimal(s: &str) -> bool {
    let (mantissa, exponent) = match s.find(['e', 'E']) {
        Some(at) => (&s[..at], Some(&s[at + 1..])),
        None => (s, None),
    };
    let digits = mantissa.bytes().filter(u8::is_ascii_digit).count();
    let points = mantissa.bytes().filter(|b| *b == b'.').count();
    let mantissa_ok = digits > 0 && points <= 1 && digits + points == mantissa.len();
    let exponent_ok = exponent.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && e.bytes().all(|b| b.is_ascii_digit())
    });
    mantissa_ok && exponent_ok
}

/// `Number(text)`: whitespace trimmed, the empty string 0, a `0x`, `0o`
/// or `0b` integer, `Infinity` with its sign, or a decimal literal with
/// its sign; anything else is not a number (NaN).
#[must_use]
pub fn js_number(text: &str) -> f64 {
    let t = text.trim_matches(is_js_space);
    if t.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if t.len() > 2 && t[..2].eq_ignore_ascii_case(prefix) {
            return u128::from_str_radix(&t[2..], radix).map_or(f64::NAN, |n| n as f64);
        }
    }
    let (sign, body) = match t.as_bytes()[0] {
        b'-' => (-1.0, &t[1..]),
        b'+' => (1.0, &t[1..]),
        _ => (1.0, t),
    };
    if body == "Infinity" {
        return sign * f64::INFINITY;
    }
    if !is_decimal(body) {
        return f64::NAN;
    }
    body.parse::<f64>().map_or(f64::NAN, |n| sign * n)
}
