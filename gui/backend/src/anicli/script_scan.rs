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
    // The verdict belongs to the COMPLETE arm: a failover call
    // followed by an unconditional hard ffmpeg requirement later in
    // the same arm still requires ffmpeg, so the grant is decided at
    // arm close, not at the first invocation seen.
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
            if let Some(after_case) = strip_keyword(rest, "case") {
                let (subject, tail) = split_at_in(after_case);
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
                match strip_case_pattern(rest, "download") {
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
                if case_owners.len() == level && invokes_yt_dlp_failover(rest) {
                    saw_failover = true;
                } else if mentions_hard_ffmpeg(rest) {
                    saw_hard_ffmpeg = true;
                }
            }
        }
    }
    // A script ending with the arm still open decides the same way.
    saw_failover && !saw_hard_ffmpeg
}

/// Whether a statement piece IS the yt-dlp failover call — the arm
/// invoking `dep_ch_failover` with `yt-dlp,ffmpeg` as its first
/// argument, which is what tells the preflight that yt-dlp alone
/// suffices.
///
/// Shape-matched for the same reason the veto is (see
/// [`mentions_hard_ffmpeg`]): the script is cached, auto-updated and
/// user-editable, so `dep_ch_failover  'yt-dlp,ffmpeg'` and a bare
/// argument mean exactly what the double-quoted spelling means.
/// Missing one costs a yt-dlp-only user their download behind a
/// missing-ffmpeg modal the script itself would not have raised.
///
/// Anchored at the start of the piece so this reads an invocation
/// rather than a mention, and the argument must be the yt-dlp pair:
/// a failover over some other toolset says nothing about yt-dlp.
fn invokes_yt_dlp_failover(rest: &str) -> bool {
    const NAME: &str = "dep_ch_failover";
    const ARG: &str = "yt-dlp,ffmpeg";
    let bytes = rest.as_bytes();
    if !rest.starts_with(NAME) {
        return false;
    }
    let mut i = NAME.len();
    let ws_start = i;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    if i == ws_start {
        return false;
    }
    let quote = bytes.get(i).copied().filter(|b| *b == b'"' || *b == b'\'');
    if quote.is_some() {
        i += 1;
    }
    if !rest[i..].starts_with(ARG) {
        return false;
    }
    let after = bytes.get(i + ARG.len()).copied();
    match (quote, after) {
        (Some(q), Some(b)) => b == q,
        (Some(_), None) => false,
        (None, Some(b)) => !(b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b','),
        (None, None) => true,
    }
}

/// Whether a statement piece hard-requires ffmpeg — `dep_ch` called
/// with `ffmpeg` as its first argument.
///
/// Matched by shape rather than by literal string: the script is
/// cached, auto-updated and user-editable, so any shell-valid
/// spacing (`dep_ch  "ffmpeg"`, `dep_ch\t'ffmpeg'`, `dep_ch ffmpeg`)
/// has to read as the same requirement. Missing one would grant
/// yt-dlp capability on a script that still exits at its own hard
/// check.
///
/// The `dep_ch` occurrence must start on a word boundary, so
/// `dep_ch_failover` (whose next byte is `_`, not whitespace) and a
/// distinct command like `my_dep_ch` are both excluded; the argument
/// may be single-quoted, double-quoted or bare, and must end at its
/// quote or a word boundary so `ffmpeg-bin` doesn't count.
///
/// It must also be EXECUTED — quoted text and non-command positions
/// are data. An earlier revision matched anywhere in the piece on
/// the theory that over-matching "only errs toward requiring
/// ffmpeg, which every script satisfies"; that reads the cost
/// backwards. The veto's consequence lands on the USER, as the
/// missing-ffmpeg modal this branch exists to stop raising, so a
/// `printf 'dep_ch ffmpeg'` diagnostic or a `hint=dep_ch` value
/// would block a yt-dlp-only install on a script that never checks.
/// The nested-arm shapes the loose match was protecting are still
/// covered: an arm pattern's `)`, a `;` or `&&` separator, and the
/// `if`/`then`-style introducers all open a command position.
fn mentions_hard_ffmpeg(rest: &str) -> bool {
    const NAME: &str = "dep_ch";
    let bytes = rest.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    unquoted_command_starts(rest).any(|at| {
        if at > 0 && is_word(bytes[at - 1]) {
            return false;
        }
        if !rest[at..].starts_with(NAME) {
            return false;
        }
        let mut i = at + NAME.len();
        // At least one space or tab must separate name from argument.
        let ws_start = i;
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i == ws_start {
            return false;
        }
        let quote = bytes.get(i).copied().filter(|b| *b == b'"' || *b == b'\'');
        if quote.is_some() {
            i += 1;
        }
        if !rest[i..].starts_with("ffmpeg") {
            return false;
        }
        let after = bytes.get(i + "ffmpeg".len()).copied();
        match (quote, after) {
            (Some(q), Some(b)) => b == q,
            (Some(_), None) => false,
            (None, Some(b)) => !is_word(b),
            (None, None) => true,
        }
    })
}

/// The text after a case item's `<pattern>)`, or `None` if the piece
/// does not open that arm.
///
/// POSIX allows the item to be written `(pattern)` as well as
/// `pattern)`, and both `sh` and bash accept it, so the opening paren
/// is case-item syntax rather than part of the pattern. Blanks may
/// sit inside the parens. The closing `)` is still required, which
/// keeps the match on a whole word: `downloadfoo)` does not open the
/// download arm.
fn strip_case_pattern<'a>(piece: &'a str, pattern: &str) -> Option<&'a str> {
    let head = piece
        .strip_prefix('(')
        .unwrap_or(piece)
        .trim_start_matches([' ', '\t']);
    head.strip_prefix(pattern)?
        .trim_start_matches([' ', '\t'])
        .strip_prefix(')')
}

/// The text after a leading shell keyword, or `None` if the piece
/// doesn't open with that keyword as a whole word. Shell keywords are
/// separated from what follows by BLANKS — one or more spaces or tabs
/// — so a literal `"case "` prefix would miss `case\t"$x" in`, valid
/// shell and the shape this repo's own tab indentation produces.
fn strip_keyword<'a>(piece: &'a str, keyword: &str) -> Option<&'a str> {
    let after = piece.strip_prefix(keyword)?;
    let trimmed = after.trim_start_matches([' ', '\t']);
    (trimmed.len() < after.len()).then_some(trimmed)
}

/// Splits a `case` statement's subject from whatever follows its `in`
/// keyword. `in` must stand as its own blank-delimited word, so a
/// subject that merely contains the letters (`case "$x_in_y" in`)
/// doesn't split early. No `in` on this piece means the keyword is
/// still to come on a later line — the subject is the whole piece and
/// the tail is empty.
fn split_at_in(after_case: &str) -> (&str, &str) {
    let bytes = after_case.as_bytes();
    let is_blank = |b: u8| b == b' ' || b == b'\t';
    for (at, _) in after_case.match_indices("in") {
        let before_is_blank = at > 0 && is_blank(bytes[at - 1]);
        let after_ends_word = bytes
            .get(at + 2)
            .is_none_or(|b| is_blank(*b) || *b == b';' || *b == b'\n');
        if before_is_blank && after_ends_word {
            return (&after_case[..at], &after_case[at + 2..]);
        }
    }
    (after_case, "")
}

/// Byte offsets in a statement piece where a command word may begin:
/// outside quotes, and either at the very start or after something
/// that ends the previous command.
///
/// Deliberately shallow — this is a scanner over one `;;`-delimited
/// piece, not a shell parser. Quote state is tracked from the start
/// of the piece, so a string carried in from an earlier line reads
/// as unquoted here; that is the pre-existing conservative direction
/// (it can only add a veto) and the `ShellScan` carry flag already
/// keeps the raw-line checks off those lines.
///
/// Command position is opened by an operator (`;`, `&`, `|`, and so
/// the `&&`/`||` pairs), a grouping opener (`(`, `{`, backquote), a
/// case-arm pattern's `)`, or a reserved word that introduces a
/// command. `=` deliberately does not: the right side of an
/// assignment is a value.
fn unquoted_command_starts(piece: &str) -> impl Iterator<Item = usize> + '_ {
    const INTRODUCERS: [&str; 11] = [
        "if", "then", "else", "elif", "do", "while", "until", "command", "eval", "exec", "time",
    ];
    let bytes = piece.as_bytes();
    let mut starts = Vec::new();
    let mut quote: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            // Single quotes have no escapes at all in the shell;
            // inside double quotes a backslash consumes the next byte
            // so an escaped quote doesn't close the string.
            if q == b'"' && b == b'\\' {
                i += 2;
                continue;
            }
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if b == b'\'' || b == b'"' {
            quote = Some(b);
            i += 1;
            continue;
        }
        if b == b'\\' {
            i += 2;
            continue;
        }
        if b != b' ' && b != b'\t' && opens_command(piece, i, &INTRODUCERS) {
            starts.push(i);
        }
        i += 1;
    }
    starts.into_iter()
}

/// Whether byte `at` in `piece` begins a statement — see
/// [`unquoted_command_starts`] for the position rules.
fn opens_command(piece: &str, at: usize, introducers: &[&str]) -> bool {
    let bytes = piece.as_bytes();
    let mut back = at;
    while back > 0 && (bytes[back - 1] == b' ' || bytes[back - 1] == b'\t') {
        back -= 1;
    }
    let Some(prev) = back.checked_sub(1).map(|j| bytes[j]) else {
        return true;
    };
    if matches!(
        prev,
        b';' | b'&' | b'|' | b'(' | b')' | b'{' | b'}' | b'`' | b'\n' | b'!'
    ) {
        return true;
    }
    // A bare word before it: only a reserved word introduces a
    // command, and only when it is itself a whole word.
    if !(prev.is_ascii_alphanumeric() || prev == b'_') {
        return false;
    }
    let word_start = piece[..back]
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
        .last()
        .map_or(back, |(j, _)| j);
    introducers.contains(&&piece[word_start..back])
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
