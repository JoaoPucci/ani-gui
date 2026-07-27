//! Heredoc delimiter parsing for the shell line scanner.

/// The word naming a heredoc's terminator, starting at `from`, with
/// shell quote removal applied across the COMPLETE word: a delimiter
/// may be assembled from quoted and unquoted fragments (`<<'E'OF`
/// names `EOF`), so parsing stops only at whitespace or an operator,
/// not at the first fragment's end. Returns the bare delimiter and
/// the index just past the word.
pub(super) fn heredoc_delimiter(line: &str, from: usize) -> (String, usize) {
    let bytes = line.as_bytes();
    let mut word = String::new();
    let mut k = from;
    while k < bytes.len() {
        match bytes[k] {
            quote @ (b'\'' | b'"') => {
                let start = k + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end] != quote {
                    end += 1;
                }
                word.push_str(&line[start..end]);
                k = (end + 1).min(bytes.len());
            }
            b'\\' => match line[k + 1..].chars().next() {
                Some(escaped) => {
                    word.push(escaped);
                    k += 1 + escaped.len_utf8();
                }
                None => break,
            },
            b if b.is_ascii_whitespace() => break,
            b';' | b'&' | b'|' | b'<' | b'>' | b'(' | b')' => break,
            _ => {
                let start = k;
                while k < bytes.len()
                    && !bytes[k].is_ascii_whitespace()
                    && !matches!(
                        bytes[k],
                        b'\'' | b'"' | b'\\' | b';' | b'&' | b'|' | b'<' | b'>' | b'(' | b')'
                    )
                {
                    k += 1;
                }
                word.push_str(&line[start..k]);
            }
        }
    }
    (word, k)
}
