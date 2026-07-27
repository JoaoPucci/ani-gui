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
    // The verdict belongs to the COMPLETE arm: a failover call
    // followed by an unconditional hard ffmpeg requirement later in
    // the same arm still requires ffmpeg, so the grant is decided at
    // arm close, not at the first invocation seen.
    const HARD_FFMPEG: [&str; 3] = [r#"dep_ch "ffmpeg""#, "dep_ch 'ffmpeg'", "dep_ch ffmpeg"];
    let mut saw_failover = false;
    let mut saw_hard_ffmpeg = false;
    let mut scan = ShellScan::default();
    let mut case_owners: Vec<bool> = Vec::new();
    // The open download arm, as the owner-stack depth it opened at.
    // Nesting is level-sensitive: a nested case's arm terminator
    // ends the inner arm, not the download arm, and an invocation
    // inside a nested case is conditional — only statements at the
    // arm's own level speak for the download flow.
    let mut in_branch: Option<usize> = None;
    // Function bodies are definitions, not the script's active flow:
    // a defined-but-never-called helper must not speak for the
    // download branch, and the real 4.15 dep check is top-level (the
    // repo-script pin enforces that from the accepting side). The
    // boundaries are line-oriented, matching the shfmt style the
    // repo enforces on the script: a definition opens at column 0
    // and its closing brace stands alone at column 0. Skipped lines
    // still feed the scanner so quote and heredoc state stay in
    // sync.
    let mut in_fn_body = false;
    for line in script_contents.lines() {
        // A line beginning inside carried state — an open string or
        // a pending heredoc body — is data: the raw-line function
        // checks below must not fire on it, only the scanner's own
        // state machine may consume it.
        let starts_clean = !scan.carrying();
        if in_fn_body {
            scan.segments(line);
            // The closing brace stands at column 0, optionally with
            // function-level redirections after it (`} >/dev/null`) —
            // POSIX allows them on the closing compound command.
            if starts_clean && line.starts_with('}') {
                in_fn_body = false;
            }
            continue;
        }
        if starts_clean && function_def_line(line) {
            scan.segments(line);
            // A one-liner body closes on its own line — redirects
            // may follow the closing brace, so the close is a brace
            // AFTER the opener, not a brace at line end. Without an
            // opener the body is still awaited on the next line.
            in_fn_body = match (line.find('{'), line.rfind('}')) {
                (Some(open), Some(close)) => close < open,
                _ => true,
            };
            continue;
        }
        for (closes_arm, segment) in scan.segments(line) {
            if closes_arm && in_branch == Some(case_owners.len()) {
                in_branch = None;
                if saw_failover && !saw_hard_ffmpeg {
                    return true;
                }
                saw_failover = false;
                saw_hard_ffmpeg = false;
            }
            let mut rest = segment.trim_start();
            if let Some(after_case) = rest.strip_prefix("case ") {
                let (subject, tail) = after_case.split_once(" in").unwrap_or((after_case, ""));
                case_owners.push(expands_player_function(subject));
                rest = tail.trim_start();
            }
            if rest.trim_end() == "esac" {
                case_owners.pop();
                if in_branch.is_some_and(|level| level > case_owners.len()) {
                    in_branch = None;
                    if saw_failover && !saw_hard_ffmpeg {
                        return true;
                    }
                    saw_failover = false;
                    saw_hard_ffmpeg = false;
                }
                continue;
            }
            if in_branch.is_none() {
                match rest.strip_prefix("download)") {
                    Some(after) if case_owners.last() == Some(&true) => {
                        in_branch = Some(case_owners.len());
                        rest = after.trim_start();
                    }
                    _ => continue,
                }
            }
            if let Some(level) = in_branch {
                // The grant is level-sensitive — a nested invocation
                // is conditional — but the VETO is not: a hard
                // requirement anywhere in the arm, inside a nested
                // platform case's own arms included, is a path that
                // may still demand ffmpeg. The veto is a contains
                // check so nested arm patterns (`Darwin) dep_ch …`)
                // can't hide it; over-matching only errs toward
                // requiring ffmpeg, which every script satisfies.
                if case_owners.len() == level && rest.starts_with(INVOCATION) {
                    saw_failover = true;
                } else if HARD_FFMPEG.iter().any(|hard| rest.contains(hard)) {
                    saw_hard_ffmpeg = true;
                }
            }
        }
    }
    // A script ending with the arm still open decides the same way.
    saw_failover && !saw_hard_ffmpeg
}

/// Whether a raw line opens a function definition at column 0 —
/// `name()` optionally followed by its body opener. The shfmt style
/// the repo enforces on the script pins definitions (and their
/// closing braces) to column 0, which is what makes line-oriented
/// body skipping sound without parsing brace depth through string
/// noise.
fn function_def_line(line: &str) -> bool {
    let Some(first) = line.as_bytes().first() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && *first != b'_' {
        return false;
    }
    let ident_len = line
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count();
    line[ident_len..].trim_start().starts_with("()")
}

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

/// Whether a case subject actually EXPANDS the player-function
/// variable. `'$player_function'` in single quotes and
/// `\$player_function` behind a backslash are literal text — a dead
/// or documentary case switching on them can never take the
/// download branch at runtime, so it cannot own it here either.
/// Double quotes expand as usual, and the braced form counts too.
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
