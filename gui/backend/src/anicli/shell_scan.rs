//! Shell-aware line splitter for the capability probe's arm-scope
//! walk. Split from `script_scan` so the character-level scanning
//! machinery (quotes, comments, heredocs) lives apart from the
//! case-arm state machine that consumes its segments.

use super::heredoc::heredoc_delimiter;

/// Line splitter for the arm-scope walk that knows just enough
/// shell: unquoted `;;`, `;`, `&`, and `|` delimit statement pieces
/// (only `;;` closes a case arm — the piece after it is tagged), an
/// unquoted `#` at the start of a word drops the rest of the line
/// as a comment, quote state carries across lines so multi-line
/// strings stay opaque, and heredoc bodies are data. Quoted or
/// heredoc text can neither close an arm nor open one.
#[derive(Default)]
pub(super) struct ShellScan {
    in_single: bool,
    in_double: bool,
    /// Heredoc delimiters announced by `<<`/`<<-` whose bodies are
    /// still open, in announcement order. While any is pending,
    /// whole lines are body text — data fed to the command, not
    /// executable shell — until the matching terminator line.
    pending_heredocs: Vec<(String, bool)>,
}

impl ShellScan {
    /// Whether multi-line state is carried into the next line: an
    /// open string or a pending heredoc body. Lines beginning in
    /// either are data, not statements — raw-line checks (function
    /// definitions, closing braces) must not fire on them.
    pub(super) fn carrying(&self) -> bool {
        self.in_single || self.in_double || !self.pending_heredocs.is_empty()
    }

    /// Each piece is `(closes_arm_before, text)`: statements split
    /// at unquoted control operators, with the flag set on the piece
    /// following a `;;` arm terminator.
    pub(super) fn segments<'a>(&mut self, line: &'a str) -> Vec<(bool, &'a str)> {
        if let Some((delimiter, strip_tabs)) = self.pending_heredocs.first() {
            let candidate = if *strip_tabs {
                line.trim_start_matches('\t')
            } else {
                line
            };
            if candidate == delimiter.as_str() {
                self.pending_heredocs.remove(0);
            }
            return Vec::new();
        }
        let bytes = line.as_bytes();
        let mut segments = Vec::new();
        let mut start = 0;
        let mut closes_arm = false;
        let mut i = 0;
        let mut at_word_start = true;
        // A line beginning inside a multi-line string is opaque up
        // to the quote's close: its leading text is string data, not
        // executable shell, and every scope check anchors at segment
        // start. If the quote never closes here, the whole line
        // yields an empty segment.
        let mut opaque = self.in_single || self.in_double;
        while i < bytes.len() {
            let b = bytes[i];
            if self.in_single {
                if b == b'\'' {
                    self.in_single = false;
                    if opaque {
                        start = i + 1;
                        opaque = false;
                    }
                }
            } else if self.in_double {
                match b {
                    b'"' => {
                        self.in_double = false;
                        if opaque {
                            start = i + 1;
                            opaque = false;
                        }
                    }
                    b'\\' => i += 1,
                    _ => {}
                }
            } else {
                match b {
                    b'\'' => self.in_single = true,
                    b'"' => self.in_double = true,
                    b'\\' => i += 1,
                    b'<' if bytes.get(i + 1) == Some(&b'<') => {
                        if bytes.get(i + 2) == Some(&b'<') {
                            // A herestring's operator, not a heredoc:
                            // consume the whole run so its tail can't
                            // re-match as `<<`.
                            while bytes.get(i) == Some(&b'<') {
                                i += 1;
                            }
                            at_word_start = false;
                            continue;
                        }
                        let mut j = i + 2;
                        let strip_tabs = bytes.get(j) == Some(&b'-');
                        if strip_tabs {
                            j += 1;
                        }
                        while matches!(bytes.get(j), Some(&b' ') | Some(&b'\t')) {
                            j += 1;
                        }
                        let (delimiter, end) = heredoc_delimiter(line, j);
                        if !delimiter.is_empty() {
                            self.pending_heredocs.push((delimiter, strip_tabs));
                        }
                        i = end;
                        at_word_start = false;
                        continue;
                    }
                    b'#' if at_word_start => {
                        segments.push((closes_arm, &line[start..i]));
                        return segments;
                    }
                    b';' if bytes.get(i + 1) == Some(&b';') => {
                        segments.push((closes_arm, &line[start..i]));
                        closes_arm = true;
                        i += 1;
                        start = i + 1;
                    }
                    b';' | b'&' | b'|' => {
                        segments.push((closes_arm, &line[start..i]));
                        closes_arm = false;
                        start = i + 1;
                    }
                    _ => {}
                }
            }
            // Control operators begin a new word without needing
            // whitespace, so `;#` starts a comment just as ` #` does.
            at_word_start =
                b.is_ascii_whitespace() || matches!(b, b';' | b'&' | b'|' | b'(' | b')');
            i += 1;
        }
        if opaque {
            segments.push((closes_arm, ""));
        } else {
            segments.push((closes_arm, &line[start..]));
        }
        segments
    }
}
