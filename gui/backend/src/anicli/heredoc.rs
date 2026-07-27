//! Heredoc delimiter parsing for the shell line scanner.

/// The word naming a heredoc's terminator, starting at `from`:
/// optionally single- or double-quoted, optionally backslash-escaped,
/// otherwise running to whitespace or a shell operator. Returns the
/// bare delimiter and the index just past it.
pub(super) fn heredoc_delimiter(line: &str, from: usize) -> (String, usize) {
    let bytes = line.as_bytes();
    match bytes.get(from) {
        Some(&q) if q == b'\'' || q == b'"' => {
            let mut k = from + 1;
            while k < bytes.len() && bytes[k] != q {
                k += 1;
            }
            (line[from + 1..k].to_string(), (k + 1).min(bytes.len()))
        }
        _ => {
            let mut k = from;
            if bytes.get(k) == Some(&b'\\') {
                k += 1;
            }
            let word_start = k;
            while k < bytes.len()
                && !bytes[k].is_ascii_whitespace()
                && !matches!(bytes[k], b';' | b'&' | b'|' | b'<' | b'>' | b'(' | b')')
            {
                k += 1;
            }
            (line[word_start..k].to_string(), k)
        }
    }
}
