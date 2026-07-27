//! Arm-scoped scan of the active ani-cli script for the 4.15
//! download failover. Split from `env` so the shell-aware walk's
//! branching lives beside its own concern; `env::download_tool_names`
//! is the only consumer.

use super::shell_scan::ShellScan;

/// Whether the script's download dependency branch invokes 4.15's
/// either-tool failover. Only the `download)` arm of the case that
/// switches on `"$player_function"` governs `-d` mode: an unrelated
/// case statement — a CLI-flag dispatcher, a mode helper — may carry
/// its own download) arm and even call the failover there while the
/// real download branch still hard-requires ffmpeg. Within the arm
/// the call must BEGIN a statement, as the real script's dep check
/// does — comments, quoted diagnostics, assignments, and a no-op
/// builtin's arguments all grant nothing. The terminator `;;` ends
/// an arm wherever it appears, so each line is walked as a sequence
/// of `;;`-delimited segments; `case … in` pushes its subject on an
/// ownership stack that `esac` pops, and the download arm opens (its
/// same-segment remainder included, for one-liner arms) only under a
/// player-function owner. Comment lines cannot move the scope.
pub(crate) fn download_branch_invokes_failover(script_contents: &str) -> bool {
    const INVOCATION: &str = r#"dep_ch_failover "yt-dlp,ffmpeg""#;
    let mut scan = ShellScan::default();
    let mut case_owners: Vec<bool> = Vec::new();
    let mut in_branch = false;
    for line in script_contents.lines() {
        for (i, segment) in scan.segments(line).into_iter().enumerate() {
            if i > 0 {
                in_branch = false;
            }
            let mut rest = segment.trim_start();
            if let Some(after_case) = rest.strip_prefix("case ") {
                let (subject, tail) = after_case.split_once(" in").unwrap_or((after_case, ""));
                case_owners.push(expands_player_function(subject));
                rest = tail.trim_start();
            }
            if rest.trim_end() == "esac" {
                in_branch = false;
                case_owners.pop();
                continue;
            }
            if !in_branch {
                match rest.strip_prefix("download)") {
                    Some(after) if case_owners.last() == Some(&true) => {
                        in_branch = true;
                        rest = after.trim_start();
                    }
                    _ => continue,
                }
            }
            if rest.starts_with(INVOCATION) {
                return true;
            }
        }
    }
    false
}

/// Whether a case subject actually EXPANDS the player-function
/// variable. `'$player_function'` in single quotes and
/// `\$player_function` behind a backslash are literal text — a dead
/// or documentary case switching on them can never take the
/// download branch at runtime, so it cannot own it here either.
/// Double quotes expand as usual, and the braced form counts too.
/// Whether `text` (starting at a `$`) is an expansion of exactly the
/// player-function variable: the braced form matches whole, and the
/// unbraced form must end at a shell identifier boundary —
/// `$player_function_backup` is a different variable.
fn expansion_at(text: &str) -> bool {
    if text.starts_with("${player_function}") {
        return true;
    }
    match text.strip_prefix("$player_function") {
        Some(after) => !after
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_'),
        None => false,
    }
}

fn expands_player_function(subject: &str) -> bool {
    let bytes = subject.as_bytes();
    let mut in_single = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if in_single {
            if b == b'\'' {
                in_single = false;
            }
        } else {
            match b {
                b'\'' => in_single = true,
                b'\\' => i += 1,
                b'$' if expansion_at(&subject[i..]) => return true,
                _ => {}
            }
        }
        i += 1;
    }
    false
}
