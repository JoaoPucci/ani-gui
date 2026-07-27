//! Arm-scoped scan of the active ani-cli script for the 4.15
//! download failover. Split from `env` so the shell-aware walk's
//! branching lives beside its own concern; `env::download_tool_names`
//! is the only consumer.

/// Whether the script's download dependency branch invokes 4.15's
/// either-tool failover. Only the `download)` case arm that governs
/// `-d` mode speaks for download capability: a customized script may
/// invoke the same failover in an unrelated helper while its download
/// branch still hard-requires ffmpeg. Within the arm the call must
/// BEGIN a statement, as the real script's dep check does — comments,
/// quoted diagnostics, assignments, and a no-op builtin's arguments
/// all grant nothing. The terminator `;;` ends an arm wherever it
/// appears, so each line is walked as a sequence of `;;`-delimited
/// segments: the download arm opens at a segment starting with
/// `download)` (its remainder included, for one-liner arms) and
/// closes at the next segment boundary or at `esac`. Comment lines
/// cannot move the scope at all.
pub(crate) fn download_branch_invokes_failover(script_contents: &str) -> bool {
    const INVOCATION: &str = r#"dep_ch_failover "yt-dlp,ffmpeg""#;
    let mut scan = ShellScan::default();
    let mut in_branch = false;
    for line in script_contents.lines() {
        for (i, segment) in scan.segments(line).into_iter().enumerate() {
            if i > 0 {
                in_branch = false;
            }
            let mut rest = segment.trim_start();
            if !in_branch {
                match rest.strip_prefix("download)") {
                    Some(after) => {
                        in_branch = true;
                        rest = after.trim_start();
                    }
                    None => continue,
                }
            }
            if rest.starts_with(INVOCATION) {
                return true;
            }
            if rest.trim_end() == "esac" {
                in_branch = false;
            }
        }
    }
    false
}

/// Line splitter for the arm-scope walk that knows just enough shell:
/// `;;` delimits case-arm segments only when unquoted, an unquoted
/// `#` at the start of a word drops the rest of the line as a
/// comment, and quote state carries across lines so multi-line
/// strings stay opaque. Quoted text can neither close an arm (a `;;`
/// as command data) nor open one (a `download)` inside a string or
/// comment).
#[derive(Default)]
struct ShellScan {
    in_single: bool,
    in_double: bool,
}

impl ShellScan {
    fn segments<'a>(&mut self, line: &'a str) -> Vec<&'a str> {
        let bytes = line.as_bytes();
        let mut segments = Vec::new();
        let mut start = 0;
        let mut i = 0;
        let mut at_word_start = true;
        while i < bytes.len() {
            let b = bytes[i];
            if self.in_single {
                if b == b'\'' {
                    self.in_single = false;
                }
            } else if self.in_double {
                match b {
                    b'"' => self.in_double = false,
                    b'\\' => i += 1,
                    _ => {}
                }
            } else {
                match b {
                    b'\'' => self.in_single = true,
                    b'"' => self.in_double = true,
                    b'\\' => i += 1,
                    b'#' if at_word_start => {
                        segments.push(&line[start..i]);
                        return segments;
                    }
                    b';' if bytes.get(i + 1) == Some(&b';') => {
                        segments.push(&line[start..i]);
                        i += 1;
                        start = i + 1;
                    }
                    _ => {}
                }
            }
            at_word_start = b.is_ascii_whitespace();
            i += 1;
        }
        segments.push(&line[start..]);
        segments
    }
}
